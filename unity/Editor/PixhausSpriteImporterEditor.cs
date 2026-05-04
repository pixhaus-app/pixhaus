using UnityEditor;
using UnityEditor.AssetImporters;
using UnityEngine;

namespace Pixhaus.Editor
{
    [CustomEditor(typeof(PixhausSpriteImporter))]
    public class PixhausSpriteImporterEditor : ScriptedImporterEditor
    {
        private SerializedProperty pixelsPerUnit;
        private SerializedProperty filterMode;
        private SerializedProperty generateMipMaps;
        private SerializedProperty spriteMeshType;

        public override void OnEnable()
        {
            base.OnEnable();
            pixelsPerUnit = serializedObject.FindProperty("pixelsPerUnit");
            filterMode = serializedObject.FindProperty("filterMode");
            generateMipMaps = serializedObject.FindProperty("generateMipMaps");
            spriteMeshType = serializedObject.FindProperty("spriteMeshType");
        }

        public override void OnInspectorGUI()
        {
            serializedObject.Update();

            EditorGUILayout.LabelField("Sprite", EditorStyles.boldLabel);
            EditorGUILayout.PropertyField(pixelsPerUnit, new GUIContent("Pixels Per Unit"));
            EditorGUILayout.PropertyField(spriteMeshType, new GUIContent("Mesh Type"));

            EditorGUILayout.Space();
            EditorGUILayout.LabelField("Texture", EditorStyles.boldLabel);
            EditorGUILayout.PropertyField(filterMode, new GUIContent("Filter Mode"));
            EditorGUILayout.PropertyField(generateMipMaps, new GUIContent("Generate Mip Maps"));

            serializedObject.ApplyModifiedProperties();
            ApplyRevertGUI();
        }
    }
}
