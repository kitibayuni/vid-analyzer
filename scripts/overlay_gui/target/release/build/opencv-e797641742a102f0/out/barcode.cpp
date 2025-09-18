#include "ocvrs_common.hpp"
#include <opencv2/barcode.hpp>
#include "barcode_types.hpp"

extern "C" {
void cv_barcode_BarcodeDetector_BarcodeDetector_const_stringR_const_stringR(const char* prototxt_path, const char* model_path, Result<cv::barcode::BarcodeDetector*>* ocvrs_return) {
	try {
		cv::barcode::BarcodeDetector* ret = new cv::barcode::BarcodeDetector(std::string(prototxt_path), std::string(model_path));
		Ok(ret, ocvrs_return);
	} OCVRS_CATCH(ocvrs_return);
}

void cv_barcode_BarcodeDetector_BarcodeDetector(Result<cv::barcode::BarcodeDetector*>* ocvrs_return) {
	try {
		cv::barcode::BarcodeDetector* ret = new cv::barcode::BarcodeDetector();
		Ok(ret, ocvrs_return);
	} OCVRS_CATCH(ocvrs_return);
}

void cv_barcode_BarcodeDetector_detect_const_const__InputArrayR_const__OutputArrayR(const cv::barcode::BarcodeDetector* instance, const cv::_InputArray* img, const cv::_OutputArray* points, Result<bool>* ocvrs_return) {
	try {
		bool ret = instance->detect(*img, *points);
		Ok(ret, ocvrs_return);
	} OCVRS_CATCH(ocvrs_return);
}

void cv_barcode_BarcodeDetector_decode_const_const__InputArrayR_const__InputArrayR_vectorLstringGR_vectorLBarcodeTypeGR(const cv::barcode::BarcodeDetector* instance, const cv::_InputArray* img, const cv::_InputArray* points, std::vector<std::string>* decoded_info, std::vector<cv::barcode::BarcodeType>* decoded_type, Result<bool>* ocvrs_return) {
	try {
		bool ret = instance->decode(*img, *points, *decoded_info, *decoded_type);
		Ok(ret, ocvrs_return);
	} OCVRS_CATCH(ocvrs_return);
}

void cv_barcode_BarcodeDetector_detectAndDecode_const_const__InputArrayR_vectorLstringGR_vectorLBarcodeTypeGR_const__OutputArrayR(const cv::barcode::BarcodeDetector* instance, const cv::_InputArray* img, std::vector<std::string>* decoded_info, std::vector<cv::barcode::BarcodeType>* decoded_type, const cv::_OutputArray* points, Result<bool>* ocvrs_return) {
	try {
		bool ret = instance->detectAndDecode(*img, *decoded_info, *decoded_type, *points);
		Ok(ret, ocvrs_return);
	} OCVRS_CATCH(ocvrs_return);
}

void cv_barcode_BarcodeDetector_detectAndDecode_const_const__InputArrayR_vectorLstringGR_vectorLBarcodeTypeGR(const cv::barcode::BarcodeDetector* instance, const cv::_InputArray* img, std::vector<std::string>* decoded_info, std::vector<cv::barcode::BarcodeType>* decoded_type, Result<bool>* ocvrs_return) {
	try {
		bool ret = instance->detectAndDecode(*img, *decoded_info, *decoded_type);
		Ok(ret, ocvrs_return);
	} OCVRS_CATCH(ocvrs_return);
}

void cv_barcode_BarcodeDetector_delete(cv::barcode::BarcodeDetector* instance) {
		delete instance;
}

}
